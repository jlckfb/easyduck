"""WebSocket transport for a single authenticated EasyDuck PCM session."""
import base64
import binascii
from collections import deque
import hashlib
import select
import time

from wsproto.connection import Connection, ConnectionType
from wsproto.events import BytesMessage, CloseConnection, Ping, TextMessage

GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11'


def upgrade(handler, intercom, packet_size, intercom_error, *, buffer_frames=22,
            io_timeout=1, drop_backlogged_audio=False, allow_mic_boost=False):
    key = handler.headers.get('Sec-WebSocket-Key', '')
    try:
        valid_key = len(base64.b64decode(key, validate=True)) == 16
    except (ValueError, binascii.Error):
        valid_key = False
    if (not valid_key or handler.headers.get('Sec-WebSocket-Version') != '13'
            or handler.headers.get('Upgrade', '').lower() != 'websocket'
            or 'upgrade' not in {value.strip() for value in handler.headers.get('Connection', '').lower().split(',')}):
        handler.send_error(400)
        return

    reply = base64.b64encode(hashlib.sha1((key + GUID).encode('ascii')).digest()).decode('ascii')
    handler.protocol_version = 'HTTP/1.1'
    handler.close_connection = True
    handler.send_response(101)
    handler.send_header('Upgrade', 'websocket')
    handler.send_header('Connection', 'Upgrade')
    handler.send_header('Sec-WebSocket-Accept', reply)
    handler.end_headers()
    handler.wfile.flush()
    _stream(handler.connection, intercom, packet_size, intercom_error, buffer_frames,
            io_timeout, drop_backlogged_audio, allow_mic_boost)


def _stream(sock, intercom, packet_size, intercom_error, buffer_frames,
            io_timeout, drop_backlogged_audio=False, allow_mic_boost=False):
    ws = Connection(ConnectionType.SERVER)
    token = None
    mode = 'listen'
    claimed = False
    last_received = time.monotonic()
    message = bytearray()
    queued_audio = deque()
    next_play = None
    closed_by = 'protocol error'
    io_operation = 'receive'
    pending_audio = b''
    sock.settimeout(io_timeout)

    def send(event):
        nonlocal io_operation, pending_audio
        io_operation = 'send'
        if pending_audio:
            sock.sendall(pending_audio)
            pending_audio = b''
        sock.sendall(ws.send(event))
        io_operation = 'receive'

    def flush_audio():
        nonlocal io_operation, pending_audio
        io_operation = 'send'
        sock.setblocking(False)
        try:
            sent = sock.send(pending_audio)
            if not sent:
                raise ConnectionError('audio socket closed')
            pending_audio = pending_audio[sent:]
        except BlockingIOError:
            pass
        finally:
            sock.settimeout(io_timeout)
        io_operation = 'receive'

    try:
        while time.monotonic() - last_received < intercom.session_timeout:
            if pending_audio:
                flush_audio()
            if next_play is not None:
                timeout = min(.15, max(0, next_play - time.monotonic()))
            else:
                timeout = 0 if token and mode == 'listen' else .15
            if (hasattr(sock, 'pending') and sock.pending()) or select.select([sock], [], [], timeout)[0]:
                chunk = sock.recv(4096)
                if not chunk:
                    closed_by = 'peer EOF'
                    break
                ws.receive_data(chunk)
                for event in ws.events():
                    if isinstance(event, CloseConnection):
                        closed_by = f'peer close ({event.code})'
                        if event.reason in {'manual', 'app_paused', 'address_change', 'audio_focus',
                                            'playback_error', 'recording_error', 'connection_error'}:
                            closed_by += f' {event.reason}'
                        send(event.response())
                        return
                    if isinstance(event, Ping):
                        send(event.response())
                        continue
                    if not isinstance(event, (TextMessage, BytesMessage)):
                        continue
                    message.extend(event.data.encode('utf-8') if isinstance(event, TextMessage) else event.data)
                    if len(message) > packet_size:
                        send(CloseConnection(code=1009))
                        return
                    if not event.message_finished:
                        continue
                    last_received = time.monotonic()
                    data = bytes(message)
                    message.clear()
                    if token is None:
                        if not isinstance(event, TextMessage) or len(data) != 48:
                            send(CloseConnection(code=1008))
                            return
                        candidate = data.decode('ascii')
                        if not intercom.claim_stream(candidate):
                            send(CloseConnection(code=1008))
                            return
                        claimed = True
                        token = candidate
                        send(TextMessage(data='ready'))
                    elif isinstance(event, TextMessage) and data in (b'listen', b'talk', b'ping'):
                        if data != b'ping':
                            mode = data.decode('ascii')
                        if mode == 'talk' or queued_audio:
                            intercom.exchange(token, 'talk', b'')
                    elif allow_mic_boost and isinstance(event, TextMessage) and data.startswith(b'mic-boost:'):
                        level = data[len(b'mic-boost:'):]
                        if level not in (b'0', b'1', b'2', b'3'):
                            send(TextMessage(data='mic-boost-error'))
                            continue
                        try:
                            intercom.set_mic_boost(token, int(level))
                        except intercom_error:
                            send(TextMessage(data='mic-boost-error'))
                        else:
                            send(TextMessage(data=data.decode('ascii')))
                    elif isinstance(event, BytesMessage) and mode == 'talk' and data and len(data) % 2 == 0:
                        if len(queued_audio) >= 36:
                            queued_audio.popleft()
                        queued_audio.append(data)
                    else:
                        send(CloseConnection(code=1008))
                        return
            # A short PCM buffer absorbs WAN bursts before the speaker begins.
            # Returning to listen drains the tail of a short utterance first.
            if queued_audio and next_play is None and (len(queued_audio) >= buffer_frames or mode == 'listen'):
                # Prime the ALSA pipe before pacing frames against a fixed clock.
                next_play = time.monotonic() - .12
            if queued_audio and next_play is not None and time.monotonic() >= next_play:
                frame = queued_audio.popleft()
                intercom.exchange(token, 'talk', frame)
                next_play += len(frame) / 32000
            if not queued_audio:
                next_play = None
            if token and mode == 'listen' and not queued_audio:
                audio = intercom.exchange(token, mode, b'')
                if audio and not drop_backlogged_audio:
                    send(BytesMessage(data=audio))
                elif audio and not pending_audio:
                    # Keep at most one frame so a slow receiver cannot block heartbeats.
                    pending_audio = ws.send(BytesMessage(data=audio))
                    flush_audio()
        else:
            closed_by = 'heartbeat timeout'
    except intercom_error as error:
        closed_by = f'audio session error ({error.status})'
    except (OSError, UnicodeError) as error:
        closed_by = f'transport {io_operation} error ({type(error).__name__})'
    finally:
        if claimed:
            print(f'Intercom closed in {mode}: {closed_by} '
                  f'({time.monotonic() - last_received:.2f}s since client message)', flush=True)
            intercom.release_stream(token)

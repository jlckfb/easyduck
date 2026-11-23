"""Local voice endpoint for the EasyDuck Android controller."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from ipaddress import IPv4Address, IPv4Network
import json
import signal
import threading
from urllib.parse import urlsplit

from intercom import Intercom, IntercomError, PACKET
from intercom_stream import upgrade

LOCAL_NETWORKS = tuple(map(IPv4Network, ('10.0.0.0/8', '172.16.0.0/12',
                                          '192.168.0.0/16', '169.254.0.0/16', '127.0.0.0/8')))


def handler_for(intercom):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, status, value):
            payload = json.dumps(value).encode('utf-8')
            self.send_response(status)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.send_header('Cache-Control', 'no-store')
            self.send_header('Connection', 'close')
            self.end_headers()
            self.wfile.write(payload)
            self.close_connection = True

        def check_origin(self):
            host = self.headers.get('Host', '')
            return bool(host) and self.headers.get('Origin') == 'http://' + host

        def do_GET(self):
            route = urlsplit(self.path).path
            if route == '/health':
                return self.reply(200, {'audio': 'pcm-s16le', 'sampleRate': 16000})
            if route != '/api/intercom/stream':
                return self.reply(404, {})
            if not self.check_origin():
                return self.reply(403, {})
            return upgrade(self, intercom, PACKET, IntercomError, buffer_frames=6,
                           io_timeout=5, drop_backlogged_audio=True)

        def do_POST(self):
            route = urlsplit(self.path).path
            if route not in ('/api/intercom/open', '/api/intercom/close'):
                return self.reply(404, {})
            if not self.check_origin():
                return self.reply(403, {})
            try:
                size = int(self.headers.get('Content-Length', '0'))
                if self.headers.get('Transfer-Encoding'):
                    return self.reply(400, {})
                if size != 0:
                    return self.reply(400, {})
                if route == '/api/intercom/close':
                    intercom.exchange(self.headers.get('X-Intercom-Token', ''), 'close', b'')
                    return self.reply(200, {})
                token = intercom.open()
                return self.reply(200, {'token': token})
            except IntercomError as exc:
                return self.reply(exc.status, {'detail': str(exc)})
            except (ValueError, OSError):
                return self.reply(400, {})

    return Handler


class LanIntercomServer(ThreadingHTTPServer):
    def verify_request(self, request, address):
        client = IPv4Address(address[0])
        return any(client in network for network in LOCAL_NETWORKS)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--host', default='0.0.0.0')
    parser.add_argument('--port', type=int, default=9460)
    args = parser.parse_args()
    intercom = Intercom(None, mic_boost=3, session_timeout=12)
    try:
        with LanIntercomServer((args.host, args.port), handler_for(intercom)) as server:
            def stop(*_):
                threading.Thread(target=server.shutdown, daemon=True).start()
            signal.signal(signal.SIGINT, stop)
            signal.signal(signal.SIGTERM, stop)
            server.serve_forever()
    finally:
        intercom.shutdown()


if __name__ == '__main__':
    main()

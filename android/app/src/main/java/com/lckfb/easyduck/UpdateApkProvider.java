package com.lckfb.easyduck;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;

import java.io.File;
import java.io.FileNotFoundException;

public final class UpdateApkProvider extends ContentProvider {
    static final String APK_TYPE = "application/vnd.android.package-archive";

    @Override public boolean onCreate() { return true; }

    private File file(Uri uri) {
        if (!("/" + AppUpdates.APK_NAME).equals(uri.getPath())) {
            throw new IllegalArgumentException("Unknown APK");
        }
        return AppUpdates.downloadedApk(getContext());
    }

    @Override public String getType(Uri uri) {
        file(uri);
        return APK_TYPE;
    }

    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        if (!"r".equals(mode)) throw new FileNotFoundException("Read only");
        return ParcelFileDescriptor.open(file(uri), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection, String[] args, String sort) {
        File apk = file(uri);
        if (projection == null) projection = new String[]{OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE};
        MatrixCursor cursor = new MatrixCursor(projection);
        if (!apk.isFile()) return cursor;
        MatrixCursor.RowBuilder row = cursor.newRow();
        for (String column : projection) {
            row.add(switch (column) {
                case OpenableColumns.DISPLAY_NAME -> apk.getName();
                case OpenableColumns.SIZE -> apk.length();
                default -> null;
            });
        }
        return cursor;
    }

    @Override public Uri insert(Uri uri, ContentValues values) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] args) {
        throw new UnsupportedOperationException();
    }
    @Override public int delete(Uri uri, String selection, String[] args) {
        throw new UnsupportedOperationException();
    }
}

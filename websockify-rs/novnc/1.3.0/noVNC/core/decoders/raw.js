/*
 * noVNC: HTML5 VNC client
 * Copyright (C) 2019 The noVNC Authors
 * Licensed under MPL 2.0 (see LICENSE.txt)
 *
 * See README.md for usage and integration instructions.
 *
 */

export default class RawDecoder {
    constructor() {
        this._lines = 0;
    }

    decodeRect(x, y, width, height, sock, display, depth) {
        if ((width === 0) || (height === 0)) {
            return true;
        }

        if (this._lines === 0) {
            this._lines = height;
        }

        const pixelSize = depth === 16 ? 2 : (depth === 8 ? 1 : 4);
        const bytesPerLine = width * pixelSize;

        if (sock.rQwait("RAW", bytesPerLine)) {
            return false;
        }

        const curY = y + (height - this._lines);
        const currHeight = Math.min(this._lines,
                                    Math.floor(sock.rQlen / bytesPerLine));
        const pixels = width * currHeight;

        let data = sock.rQ;
        let index = sock.rQi;

        // Convert data if needed
        if (depth == 8) {
            const newdata = new Uint8Array(pixels * 4);
            for (let i = 0; i < pixels; i++) {
                newdata[i * 4 + 0] = ((data[index + i] >> 0) & 0x3) * 255 / 3;
                newdata[i * 4 + 1] = ((data[index + i] >> 2) & 0x3) * 255 / 3;
                newdata[i * 4 + 2] = ((data[index + i] >> 4) & 0x3) * 255 / 3;
                newdata[i * 4 + 3] = 255;
            }
            data = newdata;
            index = 0;
        } else if (depth === 16) {
            // Native little-endian RGB565, expanded to canvas RGBA.
            const rgba = new Uint8Array(pixels * 4);
            for (let i = 0; i < pixels; i++) {
                const offset = index + i * 2;
                const pixel = data[offset] | (data[offset + 1] << 8);
                rgba[i * 4] = (pixel >> 11) * 255 / 31;
                rgba[i * 4 + 1] = ((pixel >> 5) & 63) * 255 / 63;
                rgba[i * 4 + 2] = (pixel & 31) * 255 / 31;
            }
            data = rgba;
            index = 0;
        }

        // Make sure the image is fully opaque, starting at the rectangle's offset.
        for (let i = 0; i < pixels; i++) {
            data[index + i * 4 + 3] = 255;
        }

        display.blitImage(x, curY, width, currHeight, data, index);
        sock.rQskipBytes(currHeight * bytesPerLine);
        this._lines -= currHeight;
        if (this._lines > 0) {
            return false;
        }

        return true;
    }
}

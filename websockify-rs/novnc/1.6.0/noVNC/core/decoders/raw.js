/*
 * noVNC: HTML5 VNC client
 * Copyright (C) 2019 The noVNC authors
 * Licensed under MPL 2.0 (see LICENSE.txt)
 *
 * See README.md for usage and integration instructions.
 *
 */

export default class RawDecoder {
    constructor() {
        this._lines = 0;
    }

    decodeRect(x, y, width, height, sock, display, depth, pixelFormat) {
        if ((width === 0) || (height === 0)) {
            return true;
        }

        if (this._lines === 0) {
            this._lines = height;
        }

        const pixelSize = depth == 8 ? 1 : (depth == 16 ? 2 : 4);
        const bytesPerLine = width * pixelSize;

        while (this._lines > 0) {
            if (sock.rQwait("RAW", bytesPerLine)) {
                return false;
            }

            const curY = y + (height - this._lines);

            let data = sock.rQshiftBytes(bytesPerLine, false);

            // Convert data if needed
            if (depth == 8) {
                const newdata = new Uint8Array(width * 4);
                for (let i = 0; i < width; i++) {
                    newdata[i * 4 + 0] = ((data[i] >> 0) & 0x3) * 255 / 3;
                    newdata[i * 4 + 1] = ((data[i] >> 2) & 0x3) * 255 / 3;
                    newdata[i * 4 + 2] = ((data[i] >> 4) & 0x3) * 255 / 3;
                    newdata[i * 4 + 3] = 255;
                }
                data = newdata;
            } else if (depth == 16) {
                // Expand the server's native 16bpp pixels (usually RGB565)
                // to the RGBA buffer that Display.blitImage expects.
                const pf = pixelFormat || {
                    bigEndian: 0,
                    redMax: 31,
                    greenMax: 63,
                    blueMax: 31,
                    redShift: 11,
                    greenShift: 5,
                    blueShift: 0,
                };
                const newdata = new Uint8Array(width * 4);
                for (let i = 0; i < width; i++) {
                    const lo = data[i * 2];
                    const hi = data[i * 2 + 1];
                    const px = pf.bigEndian ? (lo << 8) | hi : lo | (hi << 8);
                    newdata[i * 4 + 0] =
                        ((px >> pf.redShift) & pf.redMax) * 255 / pf.redMax;
                    newdata[i * 4 + 1] =
                        ((px >> pf.greenShift) & pf.greenMax) * 255 / pf.greenMax;
                    newdata[i * 4 + 2] =
                        ((px >> pf.blueShift) & pf.blueMax) * 255 / pf.blueMax;
                    newdata[i * 4 + 3] = 255;
                }
                data = newdata;
            }

            // Max sure the image is fully opaque
            for (let i = 0; i < width; i++) {
                data[i * 4 + 3] = 255;
            }

            display.blitImage(x, curY, width, 1, data, 0);
            this._lines--;
        }

        return true;
    }
}

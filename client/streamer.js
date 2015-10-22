var Streamers;

(function() {
    "use strict";
    var defaults = {
        server: "ws://127.0.0.1:9998/",
        canvas: ".streamer-video",
        image: ".full-picture",
    };
    var FULL_IMAGE_PREFIX = 0x55;
    var CAMERA_IN_USE = 0x33;
    var MAX_PACKET_SIZE = 65536;

    Streamers = function(options) {
        var opts = _.extend(defaults, options);
        var protocol = "jpeg-meta";
        var self = this;
        var paused = false;
        var taking_picture = false;
        var waiting_for_highres = false;
        var want_picture = [];

        var canvases;
        if (_.isString(opts.canvas)) {
            canvases = document.querySelectorAll(opts.canvas);
        } else if (_.isFunction(opts.canvas)) {
            canvases = opts.canvas();
        } else {
            throw new Error("'canvas' must be a function or a CSS selector");
        }
        var contexts = _.map(canvases, function(canvas) {
            return canvas.getContext("2d");
        });

        var conn = new WebSocket(opts.server, protocol);
        conn.binaryType = "blob";

        var jpgToCanvas = function(jpeg, context) {
            var img_url = URL.createObjectURL(jpeg);
            var image = new Image();
            image.onload = function() {
                // URL.revokeObjectURL(img_url);
                context.drawImage(image, 0, 0, image.width, image.height);
            };
            image.src = img_url;
        };

        var handleServerMessage = function(flag) {
            if (flag === FULL_IMAGE_PREFIX) {
                console.log("HIGH RES COMMING");
            } else if (flag === CAMERA_IN_USE) {
                // waiting_for_highres = true;
                console.log("CAMERA IN USE");
            }
        };

        var buffer = [];
        conn.onmessage = function(event) {
            if (event.data.size === 1 && buffer.length === 0) {
                // Use a file reader to decode because js is awful
                // https://jsperf.com/array-buffer-blob
                var reader = new FileReader();
                reader.onload = function() {
                    var generated = reader.result;
                    var view = new Uint8Array(generated);
                    handleServerMessage(view[0]);
                };
                reader.readAsArrayBuffer(event.data);
            } else {
                buffer.push(event.data);
                if (event.data.size < MAX_PACKET_SIZE) {
                    var blob = new Blob(buffer, {
                        type: "image/jpeg"
                    });
                    buffer = [];
                    var BIG = 219632;
                    if (blob.size > 100000) {
                        console.log("GOT BIG BLOB", blob.size);
                        var img_url = URL.createObjectURL(blob);
                        _.each(want_picture, function(callback) {
                            callback(img_url);
                        });
                        waiting_for_highres = false;
                        taking_picture = false;
                        want_picture = [];
                    } else if (!paused) {
                        _.each(contexts, jpgToCanvas.bind(undefined, blob));
                    }
                }
            }
        };

        self.photograph = function(done) {
            want_picture.push(done);
            if (taking_picture) {
                return;
            }
            conn.send("capture");
            taking_picture = true;
        };

        self.pause = function() {
            if (paused) {
                return;
            }
            conn.send("pause");
            paused = true;
        };

        self.resume = function() {
            if (!paused) {
                return;
            }
            conn.send("resume");
            paused = false;
        };
    };
})();

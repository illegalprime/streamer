var streamers;

(function() {
    "use strict";
    var defaults = {
        server: "ws://127.0.0.1:9998/",
        canvas: ".streamer-video",
    };

    streamers = function(options) {
        var opts = _.extend(defaults, options);
        var protocol = "jpeg-meta";

        var canvases;
        if (_.isString(opts.canvas)) {
            canvases = document.querySelectorAll(opts.canvas);
        } else if (_.isFunction(opts.canvas)) {
            canvases = opts.canvas();
        } else {
            throw new Error("canvas field must be a function or a CSS selector");
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
                URL.revokeObjectURL(img_url);
                context.drawImage(image, 0, 0, image.width, image.height);
            };
            image.src = img_url;
        };

        conn.onmessage = function(event) {
            _.each(contexts, jpgToCanvas.bind(undefined, event.data));
        };
    };
})();

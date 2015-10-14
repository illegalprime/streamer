#!/usr/bin/env node
"use strict";

var ws = require("ws");
var samples = require("./res/samples.js");
var Camera = require("v4l2camera").Camera;
var EventEmitter = require("events").EventEmitter;
var Jpeg = require("jpeg-fresh").Jpeg;

var emitter = new EventEmitter();

var camera = new Camera("/dev/video1");
camera.start();
var interval = Math.round((1 / 30) * 1000);
setTimeout(function reframe() {
    camera.capture(function(success) {
        if (success) {
            var frame = new Buffer(camera.toRGB());
            var jpeg = new Jpeg(frame.data, camera.width, camera.height, "rgb");
            var encoded = jpeg.encodeSync();
            emitter.emit("frame", encoded);
        }
        setTimeout(reframe, interval);
    })
}, interval); // 30 FPS

var Server = new ws.Server({
    port: 9998,
});

Server.on("connection", function(client) {
    emitter.on("frame", function(frame) {
        client.send(frame, {
            binary: true,
            mask: true,
        });
    });
});

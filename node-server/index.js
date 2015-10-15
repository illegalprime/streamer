#!/usr/bin/env node
"use strict";

var ws = require("ws");
var EventEmitter = require("events").EventEmitter;
var net = require("net");
var path = require("path");
var spawn = require("child_process").spawn;

var v4l2tcp = path.join(__dirname, "./v4l2tcp/target/release/v4l2tcp");

var CAMERA = "/dev/video1";
var CAM_PORT = 9997;
var CAM_HOST = "127.0.0.1";

var emitter = new EventEmitter();
var camera = new net.Socket();
var Server = new ws.Server({
    port: 9998,
});

(function() {
    spawn(v4l2tcp, [CAMERA, CAM_HOST + ":" + CAM_PORT]);
    var clients = 0;
    while (true) {
        try {
            camera.connect(CAM_PORT, CAM_HOST);
            break;
        } catch(err) {
            continue;
        }
    }
    camera.on("data", function(data) {
        emitter.emit("frame", data);
    });
    Server.on("connection", function(client) {
        clients++;
        if (clients === 1) {
            // TODO
        } else if (clients === emitter.getMaxListeners()) {
            return;
        }
        var send_frame = function(frame) {
            try {
                client.send(frame, {
                    binary: true,
                    mask: false,
                });
            } catch (err) {
                emitter.removeListener("frame", send_frame);
                clients--;
                if (clients === 0) {
                    // TODO
                }
            }
        };
        emitter.on("frame", send_frame);
    });
})();

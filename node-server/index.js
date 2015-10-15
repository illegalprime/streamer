#!/usr/bin/env node
"use strict";

var ws = require("ws");
var EventEmitter = require("events").EventEmitter;
var net = require("net");

var CAMERA_PORT = 9997;
var LOCALHOST = "127.0.0.1";

var emitter = new EventEmitter();
var camera = new net.Socket();
var Server = new ws.Server({
    port: 9998,
});

(function() {
    camera.connect(CAMERA_PORT, LOCALHOST);
    camera.on("data", function(data) {
        emitter.emit("frame", data);
    });
    Server.on("connection", function(client) {
        emitter.on("frame", function(frame) {
            client.send(frame, {
                binary: true,
                mask: true,
            });
        });
    });
})();

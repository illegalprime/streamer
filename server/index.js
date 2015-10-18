#!/usr/bin/env node
"use strict";

var ws = require("ws");
var EventEmitter = require("events").EventEmitter;
var net = require("net");
var path = require("path");
var spawn = require("child_process").spawn;

var v4l2tcp = path.join(__dirname, "./v4l2tcp/target/release/v4l2tcp");

var CAMERA = "/dev/video0";
var CAM_PORT = 9997;
var CAM_HOST = "127.0.0.1";
var MAX_LISTENERS = 10;

var emitter = new EventEmitter();
var camera = new net.Socket();
var server = new ws.Server({
    port: 9998,
});

(function() {
    // spawn(v4l2tcp, [CAMERA, CAM_HOST + ":" + CAM_PORT]);
    emitter.setMaxListeners(MAX_LISTENERS);

    // Handle counting of clients
    var clients = 0;
    var decrement_clients = function() {
        clients--;
        if (clients === 0) {
            camera.write("pause");
        }
    };
    var increment_clients = function() {
        clients++;
        if (clients === 1) {
            camera.write("resume");
        }
    };

    // Connect to TCP camera connection
    var connect = function() {
        camera.connect(CAM_PORT, CAM_HOST);
    };
    camera.on("error", function() {
        console.log("Reconnecting...");
        setTimeout(connect, 1000);
    });
    camera.on("data", function(data) {
        emitter.emit("frame", data);
    });
    connect();

    // Handle incomming clients
    server.on("connection", function(client) {
        // Boot if we have too many
        if (clients === MAX_LISTENERS) {
            return;
        }

        // Is the client paused?
        var paused = false;
        // Available camera commands
        var commands = {
            capture: camera.write.bind(camera, "capture"),
            pause: function() {
                decrement_clients();
                paused = true;
            },
            resume: function() {
                increment_clients();
                paused = false;
            },
        };

        // Add to client count
        increment_clients();

        // Send every image to this client
        var send_frame = function(frame) {
            try {
                client.send(frame, {
                    binary: true,
                    mask: false,
                });
            } catch (err) {
                emitter.removeListener("frame", send_frame);
                if (!paused) {
                    decrement_clients();
                }
            }
        };
        emitter.on("frame", send_frame);

        // Handle messages from clients
        client.on("message", function(message) {
            var comm = commands[message];
            if (comm) {
                comm();
            }
        });
    });

    // Turn off the camera server
    var shutdown = function() {
        camera.write("shutdown");
    };
})();

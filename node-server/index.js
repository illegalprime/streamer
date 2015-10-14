#!/usr/bin/env node
"use strict";

var ws = require("ws");
var samples = require("./res/samples.js");

var Server = new ws.Server({
    port: 9998,
});

Server.on("connection", function(client) {
    client.send(samples.image.u8, {
        binary: true,
        mask: true,
    });
});

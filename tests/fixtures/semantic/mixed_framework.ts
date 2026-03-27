// Semantic fixture: Mixed Express routes + EventEmitter in same file
// Expected boundaries:
//   HTTP Producers: 2 (GET /api/orders, POST /api/orders)
//   Event Producers: 1 (emit order:created)
//   Event Consumers: 1 (on order:created)
//   Total: 4

import express from "express";
import { EventEmitter } from "events";

const app = express();
const emitter = new EventEmitter();

// HTTP route: GET /api/orders
app.get("/api/orders", (req, res) => {
  res.json([]);
});

// HTTP route: POST /api/orders
app.post("/api/orders", (req, res) => {
  emitter.emit("order:created", req.body);
  res.status(201).json(req.body);
});

// Event consumer
emitter.on("order:created", (order) => {
  notifyWarehouse(order);
});

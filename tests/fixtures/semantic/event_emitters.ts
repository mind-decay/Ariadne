// Semantic fixture: Node.js EventEmitter patterns
// Expected boundaries:
//   Producers: 3 (emit user:created, emit order:shipped, publish payment:received)
//   Consumers: 3 (on user:created, addEventListener order:shipped, subscribe payment:received)
//   Total: 6

import { EventEmitter } from "events";

const emitter = new EventEmitter();

// Producer: emit user:created
emitter.emit("user:created", { id: 1 });

// Consumer: on user:created
emitter.on("user:created", (data) => {
  console.log(data);
});

// Producer: emit order:shipped
emitter.emit("order:shipped", { orderId: 42 });

// Consumer: addEventListener order:shipped
target.addEventListener("order:shipped", handler);

// Producer: publish payment:received
bus.publish("payment:received");

// Consumer: subscribe payment:received
bus.subscribe("payment:received", handlePayment);

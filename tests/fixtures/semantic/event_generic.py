# Semantic fixture: Generic Python publish/subscribe events
# Expected boundaries:
#   Producers: 2 (emit data_ready, publish task_completed)
#   Consumers: 2 (on data_ready, subscribe task_completed)
#   Total: 4

class EventBus:
    pass

bus = EventBus()

# Producer
bus.emit("data_ready", {"key": "value"})

# Consumer
bus.on("data_ready", lambda data: print(data))

# Producer
bus.publish("task_completed")

# Consumer
bus.subscribe("task_completed", handle_task)

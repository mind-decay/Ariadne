#include <stddef.h>

struct Counter {
    unsigned int value;
};

enum Tick {
    TICK_UP,
    TICK_DOWN,
};

typedef struct Counter Counter;

unsigned int counter_increment(struct Counter *c) {
    c->value += 1;
    return c->value;
}

unsigned int counter_double(struct Counter *c) {
    return counter_increment(c) + counter_increment(c);
}

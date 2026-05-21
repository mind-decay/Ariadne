#include <cstddef>

namespace sample {

class Counter {
public:
    explicit Counter(unsigned int start) : value_(start) {}

    unsigned int increment() {
        value_ += 1;
        return value_;
    }

    unsigned int value() const {
        return value_;
    }

private:
    unsigned int value_;
};

unsigned int twice(Counter &c) {
    c.increment();
    return c.increment();
}

}  // namespace sample

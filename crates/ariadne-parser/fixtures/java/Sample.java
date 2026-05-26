package com.ariadne.sample;

import java.util.ArrayList;
import java.util.List;

public class Sample {
    public interface Step {
        int next(int current);
    }

    public enum Side {
        LEFT,
        RIGHT,
    }

    public record Point(int x, int y) {}

    public static int increment(int v) {
        List<Integer> values = new ArrayList<>();
        values.add(v + 1);
        return values.get(0);
    }
}

class Helper {
    @Deprecated
    public void legacy() {}

    @Override
    public String toString() {
        return "helper";
    }
}

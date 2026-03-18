package com.example.data;

import java.util.HashMap;
import java.util.Map;

public class UserRepo {
    private final Map<String, String> users = new HashMap<>();

    public UserRepo() {
        users.put("admin", "admin@example.com");
    }

    public String findByUsername(String username) {
        return users.get(username);
    }
}

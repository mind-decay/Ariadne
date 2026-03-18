package com.example.service;

import com.example.data.UserRepo;

public class AuthService {
    private final UserRepo repo;

    public AuthService() {
        this.repo = new UserRepo();
    }

    public boolean login(String username, String password) {
        return repo.findByUsername(username) != null;
    }
}

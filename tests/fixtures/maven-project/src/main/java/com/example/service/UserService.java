package com.example.service;

import com.example.data.UserRepo;

public class UserService {
    private UserRepo userRepo;

    public String getUser(long id) {
        return userRepo.findById(id);
    }
}

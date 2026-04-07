package com.example;

import org.springframework.stereotype.Service;
import org.springframework.beans.factory.annotation.Autowired;
import com.example.UserRepository;

@Service
public class UserService {

    @Autowired
    private UserRepository userRepository;

    public String listAll() { return ""; }
    public String getById(long id) { return ""; }
    public String create(String name) { return ""; }
    public void delete(long id) {}
}

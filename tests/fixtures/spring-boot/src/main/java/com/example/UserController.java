package com.example;

import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.beans.factory.annotation.Autowired;
import com.example.UserService;

@RestController
@RequestMapping("/users")
public class UserController {

    @Autowired
    private UserService userService;

    @GetMapping
    public String listUsers() {
        return userService.listAll();
    }

    @GetMapping("/{id}")
    public String getUser(long id) {
        return userService.getById(id);
    }

    @PostMapping
    public String createUser(String name) {
        return userService.create(name);
    }

    @DeleteMapping("/{id}")
    public void deleteUser(long id) {
        userService.delete(id);
    }
}

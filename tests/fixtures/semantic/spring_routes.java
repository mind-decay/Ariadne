// Semantic fixture: Spring HTTP routes
// Expected boundaries:
//   Producers: 4 (RequestMapping /api, GetMapping /api/users, PostMapping /api/users, DeleteMapping /api/users/{id})
//   Consumers: 0
//   Total: 4

package com.example.demo;

import org.springframework.web.bind.annotation.*;

@RequestMapping("/api")
public class UserController {

    @GetMapping("/api/users")
    public List<User> getUsers() {
        return userService.findAll();
    }

    @PostMapping("/api/users")
    public User createUser(@RequestBody User user) {
        return userService.save(user);
    }

    @DeleteMapping("/api/users/{id}")
    public void deleteUser(@PathVariable Long id) {
        userService.deleteById(id);
    }
}

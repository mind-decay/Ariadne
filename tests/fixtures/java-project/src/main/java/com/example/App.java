package com.example;

import com.example.service.AuthService;

public class App {
    public static void main(String[] args) {
        AuthService auth = new AuthService();
        boolean result = auth.login("admin", "secret");
        System.out.println("Login result: " + result);
    }
}

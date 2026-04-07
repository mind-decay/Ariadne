package com.example;

import jakarta.ws.rs.GET;
import jakarta.ws.rs.POST;
import jakarta.ws.rs.Path;
import jakarta.ws.rs.PathParam;
import jakarta.inject.Inject;
import com.example.UserService;

@Path("/users")
public class UserResource {

    @Inject
    private UserService userService;

    @GET
    public String listUsers() {
        return userService.listAll();
    }

    @GET
    @Path("/{id}")
    public String getUser(@PathParam("id") long id) {
        return userService.getById(id);
    }

    @POST
    public String createUser(String body) {
        return userService.create(body);
    }
}

// Semantic fixture: Express HTTP routes
// Expected boundaries:
//   Producers: 4 (GET /api/users, POST /api/users, DELETE /api/users/:id, USE /api middleware)
//   Consumers: 2 (fetch /api/users, axios GET /api/users/:id)
//   Total: 6

import express from "express";

const app = express();

// Producer: GET /api/users
app.get("/api/users", (req, res) => {
  res.json([]);
});

// Producer: POST /api/users
app.post("/api/users", (req, res) => {
  res.status(201).json(req.body);
});

// Producer: DELETE /api/users/:id
app.delete("/api/users/:id", (req, res) => {
  res.status(204).end();
});

// Producer (Both): middleware mount
app.use("/api", router);

// Consumer: fetch
async function loadUsers() {
  const resp = await fetch("/api/users");
  return resp.json();
}

// Consumer: axios
async function loadUser(id: string) {
  const resp = await axios.get("/api/users/:id");
  return resp.data;
}

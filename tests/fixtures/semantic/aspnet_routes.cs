// Semantic fixture: ASP.NET HTTP routes
// Expected boundaries:
//   Producers: 4 (Route /api/products, HttpGet /api/products, HttpPost /api/products, HttpDelete /api/products/{id})
//   Consumers: 0
//   Total: 4

using Microsoft.AspNetCore.Mvc;

[Route("/api/products")]
public class ProductsController : ControllerBase
{
    [HttpGet("/api/products")]
    public IActionResult GetAll()
    {
        return Ok(new List<Product>());
    }

    [HttpPost("/api/products")]
    public IActionResult Create([FromBody] Product product)
    {
        return Created("", product);
    }

    [HttpDelete("/api/products/{id}")]
    public IActionResult Delete(int id)
    {
        return NoContent();
    }
}

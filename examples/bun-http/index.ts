Bun.serve({
  port: 2000,
  fetch(req) {
    console.log(req.url)
    return new Response("hi")
  }
})

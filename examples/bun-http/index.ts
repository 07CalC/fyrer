Bun.serve({
  port: 2000,
  fetch() {
    // return new Response("hi")
    return new Response(process.env.HELLO);

  },
  hostname: "localhost"
})



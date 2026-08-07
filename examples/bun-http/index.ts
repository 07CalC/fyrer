
while (true) {
  console.log("This is a log message.");
  console.warn("This is a warning message.");
  console.error("This is an error message.");
  await Bun.sleep(1000)
}

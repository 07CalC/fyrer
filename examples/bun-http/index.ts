
let i = 1;

while (true) {
  console.log(i);
  i++;
  await Bun.sleep(1000);
}

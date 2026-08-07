


try {
  someFunctionThatMightThrow();
} catch (error) {
  console.error('An error occurred:', error);
  process.exit(error instanceof Error ? error.message || 1 : 1);
}

function someFunctionThatMightThrow() {
  if (Math.random() < 0.5) {
    throw new Error('This is a simulated error.');
  } else {
    console.log('Function executed successfully.');
  }
}

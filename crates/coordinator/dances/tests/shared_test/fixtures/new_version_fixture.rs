// Provide the holon to clone from and the modified cloned holon into the test step
// the test executor creates the holon to clone, commits it (which adds it to the TestState)
// iterate through the TestState, get the HolonId from the saved holon, clone it, modify its
// properties based on the expected holon, commit again, then check the TestState to confirm
// it contains the expected holon
import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    threads: false,
    testTimeout: 60*1000*3, // 3  mins
    //exclude: ["all-holon-nodes.test.ts","holon-node-to-holon-nodes.test.ts","holon-node.test.ts","holon.test.ts"]
  },
})


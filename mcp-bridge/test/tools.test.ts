import { mkdtemp, mkdir, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { listDir, readFile, searchFiles } from '../src/tools.js'

const tempRoots: string[] = []

async function createFixture(): Promise<{ root: string }> {
  const root = await mkdtemp(path.join(os.tmpdir(), 'aop-bridge-test-'))
  tempRoots.push(root)

  await mkdir(path.join(root, 'src'), { recursive: true })
  await mkdir(path.join(root, 'docs'), { recursive: true })
  await writeFile(path.join(root, 'src', 'main.ts'), 'export const hello = "world"\n')
  await writeFile(path.join(root, 'docs', 'README.md'), 'Bridge test content\n')

  return { root }
}

afterEach(async () => {
  await Promise.all(tempRoots.splice(0).map((root) => rm(root, { force: true, recursive: true })))
})

describe('bridge local tools', () => {
  it('lists project directories', async () => {
    const { root } = await createFixture()
    const result = await listDir(root, '.')

    expect(result.cwd).toBe('.')
    expect(result.entries.some((entry) => entry.name === 'src' && entry.isDir)).toBe(true)
  })

  it('reads files from target project', async () => {
    const { root } = await createFixture()
    const result = await readFile(root, 'src/main.ts')

    expect(result.path).toBe('src/main.ts')
    expect(result.content).toContain('hello')
  })

  it('searches by file path and file content', async () => {
    const { root } = await createFixture()
    const pathMatches = await searchFiles(root, 'README')
    const contentMatches = await searchFiles(root, 'world')

    expect(pathMatches.matches.some((match) => match.path === 'docs/README.md')).toBe(true)
    expect(contentMatches.matches.some((match) => match.path === 'src/main.ts')).toBe(true)
  })

  it('rejects traversal paths that include dot segments', async () => {
    const { root } = await createFixture()
    await expect(readFile(root, '../outside.ts')).rejects.toThrow('SECURITY_VIOLATION')
  })

  it('rejects paths that start with tilde', async () => {
    const { root } = await createFixture()
    await expect(listDir(root, '~/secrets')).rejects.toThrow('SECURITY_VIOLATION')
  })

  it('rejects symlink traversal when links are supported by the environment', async () => {
    const { root } = await createFixture()
    const outsideRoot = await mkdtemp(path.join(os.tmpdir(), 'aop-bridge-symlink-'))
    tempRoots.push(outsideRoot)

    const outsideDirectory = path.join(outsideRoot, 'outside')
    await mkdir(outsideDirectory, { recursive: true })
    await writeFile(path.join(outsideDirectory, 'secret.txt'), 'classified\n')

    const linkPath = path.join(root, 'linked-outside')
    try {
      await symlink(outsideDirectory, linkPath, process.platform === 'win32' ? 'junction' : 'dir')
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code
      if (code === 'EPERM' || code === 'EACCES' || code === 'UNKNOWN') {
        return
      }
      throw error
    }

    await expect(listDir(root, 'linked-outside')).rejects.toThrow('SECURITY_VIOLATION')
  })
})

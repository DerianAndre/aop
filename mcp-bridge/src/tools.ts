import { promises as fs } from 'node:fs'
import path from 'node:path'

import type { BridgeDirEntry, DirectoryListing, SearchMatch, SearchResult, TargetFileContent } from './types.js'

function normalizeRoot(root: string): string {
  if (!root.trim()) {
    throw new Error('targetProject is required')
  }

  return path.resolve(root)
}

function normalizeRelativePath(rawPath: string | undefined): string {
  if (!rawPath || !rawPath.trim()) {
    return '.'
  }

  return rawPath.trim()
}

function toPosixRelative(root: string, absolutePath: string): string {
  const relative = path.relative(root, absolutePath)
  if (!relative || relative === '.') {
    return '.'
  }

  return relative.split(path.sep).join('/')
}

function assertWithinRoot(root: string, absolutePath: string): void {
  const relative = path.relative(root, absolutePath)
  const escapesRoot = relative.startsWith('..') || path.isAbsolute(relative)
  if (escapesRoot) {
    throw new Error(`Path '${absolutePath}' escapes target project root`)
  }
}

function resolveWithinRoot(root: string, requestedPath: string | undefined): string {
  const absoluteRoot = normalizeRoot(root)
  const safePath = normalizeRelativePath(requestedPath)
  const absoluteTarget = path.resolve(absoluteRoot, safePath)
  assertWithinRoot(absoluteRoot, absoluteTarget)
  return absoluteTarget
}

async function ensureDirectory(dirPath: string): Promise<void> {
  const stats = await fs.stat(dirPath)
  if (!stats.isDirectory()) {
    throw new Error(`Path '${dirPath}' is not a directory`)
  }
}

async function ensureFile(filePath: string): Promise<void> {
  const stats = await fs.stat(filePath)
  if (!stats.isFile()) {
    throw new Error(`Path '${filePath}' is not a file`)
  }
}

function compareEntries(a: BridgeDirEntry, b: BridgeDirEntry): number {
  if (a.isDir !== b.isDir) {
    return a.isDir ? -1 : 1
  }

  return a.name.localeCompare(b.name)
}

export async function listDir(targetProject: string, requestedPath: string | undefined): Promise<DirectoryListing> {
  const absoluteRoot = normalizeRoot(targetProject)
  const absoluteDirectory = resolveWithinRoot(absoluteRoot, requestedPath)

  await ensureDirectory(absoluteDirectory)

  const dirEntries = await fs.readdir(absoluteDirectory, { withFileTypes: true })
  const mappedEntries = await Promise.all(
    dirEntries.map(async (entry): Promise<BridgeDirEntry> => {
      const absoluteEntryPath = path.join(absoluteDirectory, entry.name)
      const isDir = entry.isDirectory()
      let size: number | null = null

      if (!isDir) {
        const stats = await fs.stat(absoluteEntryPath)
        size = stats.size
      }

      return {
        name: entry.name,
        path: toPosixRelative(absoluteRoot, absoluteEntryPath),
        isDir,
        size,
      }
    }),
  )

  mappedEntries.sort(compareEntries)

  const cwd = toPosixRelative(absoluteRoot, absoluteDirectory)
  const absoluteParent = path.resolve(absoluteDirectory, '..')
  const parent = absoluteParent === absoluteDirectory ? null : toPosixRelative(absoluteRoot, absoluteParent)

  return {
    root: absoluteRoot,
    cwd,
    parent: cwd === '.' || parent === '.' ? null : parent,
    entries: mappedEntries,
    source: 'local',
    warnings: [],
  }
}

export async function readFile(
  targetProject: string,
  requestedPath: string | undefined,
): Promise<TargetFileContent> {
  const absoluteRoot = normalizeRoot(targetProject)
  const absoluteFile = resolveWithinRoot(absoluteRoot, requestedPath)
  await ensureFile(absoluteFile)

  const [stats, content] = await Promise.all([
    fs.stat(absoluteFile),
    fs.readFile(absoluteFile, 'utf8'),
  ])

  return {
    root: absoluteRoot,
    path: toPosixRelative(absoluteRoot, absoluteFile),
    size: stats.size,
    content,
    source: 'local',
    warnings: [],
  }
}

function firstLineMatch(content: string, pattern: string): { line: number | null; preview: string | null } {
  const lines = content.split(/\r?\n/)
  const lowered = pattern.toLowerCase()

  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].toLowerCase().includes(lowered)) {
      return {
        line: index + 1,
        preview: lines[index].trim().slice(0, 180),
      }
    }
  }

  return { line: null, preview: null }
}

async function findMatchesInFile(
  absoluteRoot: string,
  absoluteFile: string,
  pattern: string,
): Promise<SearchMatch | null> {
  const relativePath = toPosixRelative(absoluteRoot, absoluteFile)
  const nameMatch = relativePath.toLowerCase().includes(pattern.toLowerCase())

  if (nameMatch) {
    return { path: relativePath, line: null, preview: null }
  }

  try {
    const content = await fs.readFile(absoluteFile, 'utf8')
    const { line, preview } = firstLineMatch(content, pattern)
    if (line !== null || preview !== null) {
      return { path: relativePath, line, preview }
    }
  } catch {
    return null
  }

  return null
}

export async function searchFiles(
  targetProject: string,
  pattern: string,
  limit = 40,
): Promise<SearchResult> {
  if (!pattern.trim()) {
    throw new Error('pattern is required for search_files')
  }

  const absoluteRoot = normalizeRoot(targetProject)
  const pending: string[] = [absoluteRoot]
  const matches: SearchMatch[] = []
  const safeLimit = Math.max(1, limit)

  while (pending.length > 0 && matches.length < safeLimit) {
    const currentDirectory = pending.pop()
    if (!currentDirectory) {
      continue
    }

    const entries = await fs.readdir(currentDirectory, { withFileTypes: true })
    for (const entry of entries) {
      if (matches.length >= safeLimit) {
        break
      }

      const absoluteEntry = path.join(currentDirectory, entry.name)
      if (entry.isDirectory()) {
        if (entry.name === '.git' || entry.name === 'node_modules' || entry.name === 'target') {
          continue
        }

        pending.push(absoluteEntry)
        continue
      }

      if (!entry.isFile()) {
        continue
      }

      const fileMatch = await findMatchesInFile(absoluteRoot, absoluteEntry, pattern)
      if (fileMatch) {
        matches.push(fileMatch)
      }
    }
  }

  return {
    root: absoluteRoot,
    pattern,
    matches,
    source: 'local',
    warnings: [],
  }
}

import { useState, type FormEvent } from 'react'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useTargetProjectConfig } from '@/hooks/useTargetProjectConfig'
import { indexTargetProject, listTargetDir, queryCodebase, readTargetFile, searchTargetFiles } from '@/hooks/useTauri'
import type { ContextChunk, DirectoryEntry, DirectoryListing, IndexProjectResult, SearchResult, TargetFileContent } from '@/types'

function entryIcon(entry: DirectoryEntry): string {
  return entry.isDir ? 'DIR' : 'FILE'
}

export function ContextView() {
  const {
    targetProject,
    setTargetProject,
    mcpCommand,
    setMcpCommand,
    mcpArgs,
    setMcpArgs,
    mcpConfig,
  } = useTargetProjectConfig()

  const [directory, setDirectory] = useState<DirectoryListing | null>(null)
  const [selectedFile, setSelectedFile] = useState<TargetFileContent | null>(null)
  const [searchPattern, setSearchPattern] = useState('')
  const [searchResult, setSearchResult] = useState<SearchResult | null>(null)
  const [semanticQuery, setSemanticQuery] = useState('')
  const [semanticResults, setSemanticResults] = useState<ContextChunk[]>([])
  const [indexResult, setIndexResult] = useState<IndexProjectResult | null>(null)

  const [isBrowsing, setIsBrowsing] = useState(false)
  const [isIndexing, setIsIndexing] = useState(false)
  const [isSemanticSearching, setIsSemanticSearching] = useState(false)
  const [feedback, setFeedback] = useState<string | null>(null)

  async function browseDirectory(dirPath = '.'): Promise<void> {
    const target = targetProject.trim()
    if (!target) {
      setFeedback('Target project path is required.')
      return
    }

    setIsBrowsing(true)
    setFeedback(null)
    try {
      const listing = await listTargetDir({
        targetProject: target,
        dirPath,
        ...mcpConfig,
      })
      setDirectory(listing)
      setSearchResult(null)
      setSelectedFile(null)
      if (listing.warnings.length > 0) {
        setFeedback(listing.warnings.join('\n'))
      }
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsBrowsing(false)
    }
  }

  async function openFile(filePath: string): Promise<void> {
    const target = targetProject.trim()
    if (!target) {
      setFeedback('Target project path is required.')
      return
    }

    setIsBrowsing(true)
    setFeedback(null)
    try {
      const file = await readTargetFile({
        targetProject: target,
        filePath,
        ...mcpConfig,
      })
      setSelectedFile(file)
      if (file.warnings.length > 0) {
        setFeedback(file.warnings.join('\n'))
      }
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsBrowsing(false)
    }
  }

  async function handleSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const target = targetProject.trim()
    const pattern = searchPattern.trim()

    if (!target) {
      setFeedback('Target project path is required.')
      return
    }
    if (!pattern) {
      setFeedback('Search pattern is required.')
      return
    }

    setIsBrowsing(true)
    setFeedback(null)
    try {
      const result = await searchTargetFiles({
        targetProject: target,
        pattern,
        limit: 30,
        ...mcpConfig,
      })
      setSearchResult(result)
      if (result.warnings.length > 0) {
        setFeedback(result.warnings.join('\n'))
      }
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsBrowsing(false)
    }
  }

  async function handleIndexProject() {
    const target = targetProject.trim()
    if (!target) {
      setFeedback('Target project path is required.')
      return
    }

    setIsIndexing(true)
    setFeedback(null)
    try {
      const result = await indexTargetProject({ targetProject: target })
      setIndexResult(result)
      setSemanticResults([])
      setFeedback(`Indexed ${result.indexedFiles} files into ${result.indexedChunks} chunks (${result.tableName}).`)
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsIndexing(false)
    }
  }

  async function handleSemanticSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const target = targetProject.trim()
    const query = semanticQuery.trim()

    if (!target) {
      setFeedback('Target project path is required.')
      return
    }
    if (!query) {
      setFeedback('Semantic query is required.')
      return
    }

    setIsSemanticSearching(true)
    setFeedback(null)
    try {
      const results = await queryCodebase({
        targetProject: target,
        query,
        topK: 5,
      })
      setSemanticResults(results)
      if (results.length === 0) {
        setFeedback('No semantic chunks found. Index project first or broaden the query.')
      }
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsSemanticSearching(false)
    }
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Target Project Browser (MCP)</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault()
              void browseDirectory('.')
            }}
          >
            <div className="space-y-2">
              <Label htmlFor="context-target-project">Target Project Path</Label>
              <Input
                id="context-target-project"
                onChange={(event) => setTargetProject(event.target.value)}
                placeholder="C:\\repo\\target-project"
                value={targetProject}
              />
            </div>

            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="context-mcp-command">MCP Command (optional)</Label>
                <Input
                  id="context-mcp-command"
                  onChange={(event) => setMcpCommand(event.target.value)}
                  placeholder="npx"
                  value={mcpCommand}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="context-mcp-args">MCP Args (optional)</Label>
                <Input
                  id="context-mcp-args"
                  onChange={(event) => setMcpArgs(event.target.value)}
                  placeholder="-y @anthropic/mcp-server-filesystem ./src"
                  value={mcpArgs}
                />
              </div>
            </div>

            <div className="flex flex-wrap gap-2">
              <Button disabled={isBrowsing} type="submit">
                {isBrowsing ? 'Loading...' : 'Load Root'}
              </Button>
              <Button disabled={isIndexing} onClick={() => void handleIndexProject()} type="button" variant="outline">
                {isIndexing ? 'Indexing...' : 'Index Project'}
              </Button>
              <span className="text-muted-foreground self-center text-sm">
                {indexResult ? `${indexResult.indexedFiles} files / ${indexResult.indexedChunks} chunks` : 'Index not run'}
              </span>
            </div>
          </form>

          <form className="space-y-2" onSubmit={handleSearch}>
            <Label htmlFor="context-search-pattern">Search Files</Label>
            <div className="flex flex-wrap gap-2">
              <Input
                id="context-search-pattern"
                onChange={(event) => setSearchPattern(event.target.value)}
                placeholder="useSession"
                value={searchPattern}
              />
              <Button disabled={isBrowsing} type="submit" variant="secondary">
                Search
              </Button>
            </div>
          </form>

          <form className="space-y-2" onSubmit={handleSemanticSearch}>
            <Label htmlFor="context-semantic-query">Semantic Query</Label>
            <div className="flex flex-wrap gap-2">
              <Input
                id="context-semantic-query"
                onChange={(event) => setSemanticQuery(event.target.value)}
                placeholder="components using session loading state"
                value={semanticQuery}
              />
              <Button disabled={isSemanticSearching} type="submit" variant="secondary">
                {isSemanticSearching ? 'Querying...' : 'Query'}
              </Button>
            </div>
          </form>

          {feedback ? <p className="text-muted-foreground text-sm whitespace-pre-wrap">{feedback}</p> : null}
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
        <Card>
          <CardHeader>
            <CardTitle>Directory</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-muted-foreground mb-2 text-xs">
              {directory ? `${directory.cwd} (source: ${directory.source})` : 'No directory loaded'}
            </p>
            <ScrollArea className="h-[360px]">
              <div className="space-y-1">
                {directory?.parent ? (
                  <Button
                    className="w-full justify-start"
                    onClick={() => void browseDirectory(directory.parent ?? '.')}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    <span className="mr-2 text-xs">DIR</span>..
                  </Button>
                ) : null}
                {directory?.entries.map((entry) => (
                  <Button
                    className="w-full justify-start"
                    key={entry.path}
                    onClick={() => (entry.isDir ? void browseDirectory(entry.path) : void openFile(entry.path))}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    <span className="mr-2 text-xs">{entryIcon(entry)}</span>
                    <span className="truncate">{entry.name}</span>
                  </Button>
                ))}
                {!directory ? <p className="text-muted-foreground text-sm">Load project root to browse files.</p> : null}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>File Preview</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-muted-foreground mb-2 text-xs">{selectedFile?.path ?? 'No file selected'}</p>
            <ScrollArea className="h-[360px]">
              <pre className="text-xs whitespace-pre-wrap">
                {selectedFile ? selectedFile.content.slice(0, 8000) : 'Select a file to preview content.'}
              </pre>
            </ScrollArea>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Search + Semantic Results</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-muted-foreground text-xs">
              Text matches: {searchResult ? searchResult.matches.length : 0} | Semantic chunks: {semanticResults.length}
            </p>
            <ScrollArea className="h-[360px]">
              <div className="space-y-2">
                {searchResult?.matches.map((match) => (
                  <button
                    className="w-full rounded-md border p-2 text-left"
                    key={`${match.path}-${match.line ?? 0}`}
                    onClick={() => void openFile(match.path)}
                    type="button"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-medium">{match.path}</span>
                      {match.line ? <span className="text-muted-foreground text-xs">line {match.line}</span> : null}
                    </div>
                    {match.preview ? <p className="text-muted-foreground mt-1 text-xs">{match.preview}</p> : null}
                  </button>
                ))}

                {semanticResults.map((chunk) => (
                  <button
                    className="w-full rounded-md border p-2 text-left"
                    key={chunk.id}
                    onClick={() => void openFile(chunk.filePath)}
                    type="button"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-medium">{chunk.filePath}</span>
                      <span className="text-muted-foreground text-xs">score {chunk.score.toFixed(3)}</span>
                    </div>
                    <p className="text-muted-foreground text-xs">
                      lines {chunk.startLine}-{chunk.endLine} | {chunk.chunkType} {chunk.name}
                    </p>
                    <p className="mt-1 text-xs whitespace-pre-wrap">{chunk.content.slice(0, 240)}</p>
                  </button>
                ))}

                {!searchResult && semanticResults.length === 0 ? (
                  <p className="text-muted-foreground text-sm">Run search or semantic query to see results.</p>
                ) : null}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}


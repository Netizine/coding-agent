// Read-only source archival and verified recovery. Never overwrites files.
package main

import (
	"archive/zip"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"
)

type Record struct {
	Path  string `json:"path"`
	Bytes int64  `json:"bytes"`
	SHA   string `json:"sha256"`
}
type Manifest struct {
	Files []Record `json:"files"`
	Count int      `json:"file_count"`
	SHA   string   `json:"sha256"`
}

func must(err error) {
	if err != nil {
		panic(err)
	}
}
func digest(data []byte) string { sum := sha256.Sum256(data); return hex.EncodeToString(sum[:]) }
func readManifest(path string) Manifest {
	data, err := os.ReadFile(path)
	must(err)
	var m Manifest
	must(json.Unmarshal(data, &m))
	var canonical strings.Builder
	last := ""
	for _, r := range m.Files {
		if r.Path <= last || r.Path == "." || r.Path == ".." || r.Bytes < 0 || strings.ContainsAny(r.Path, "\\:") || strings.HasPrefix(r.Path, "/") || filepath.ToSlash(filepath.Clean(r.Path)) != r.Path || strings.HasPrefix(r.Path, "../") {
			panic("invalid manifest path/order")
		}
		last = r.Path
		canonical.WriteString(r.Path + "\x00" + r.SHA + "\n")
	}
	if len(m.Files) != m.Count || digest([]byte(canonical.String())) != m.SHA {
		panic("manifest fingerprint mismatch")
	}
	return m
}
func main() {
	if len(os.Args) < 4 {
		panic("usage: reference <create|verify|restore> ARCHIVE MANIFEST [SOURCE|DEST]")
	}
	mode, archive := os.Args[1], os.Args[2]
	m := readManifest(os.Args[3])
	if mode == "create" {
		if len(os.Args) != 5 {
			panic("source required")
		}
		must(os.MkdirAll(filepath.Dir(archive), 0755))
		f, err := os.OpenFile(archive, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0644)
		must(err)
		defer f.Close()
		w := zip.NewWriter(f)
		defer w.Close()
		for _, r := range m.Files {
			data, err := os.ReadFile(filepath.Join(os.Args[4], r.Path))
			must(err)
			if int64(len(data)) != r.Bytes || digest(data) != r.SHA {
				panic("source mismatch: " + r.Path)
			}
			h := &zip.FileHeader{Name: r.Path, Method: zip.Deflate}
			h.SetModTime(time.Date(1980, 1, 1, 0, 0, 0, 0, time.UTC))
			h.SetMode(0644)
			out, err := w.CreateHeader(h)
			must(err)
			_, err = out.Write(data)
			must(err)
		}
		must(w.Close())
		must(f.Close())
	} else if mode != "verify" && mode != "restore" {
		panic("unknown command")
	}
	reader, err := zip.OpenReader(archive)
	must(err)
	defer reader.Close()
	expected := map[string]Record{}
	for _, r := range m.Files {
		expected[r.Path] = r
	}
	payloads := map[string][]byte{}
	for _, f := range reader.File {
		r, ok := expected[f.Name]
		if !ok {
			panic("unexpected or duplicate entry: " + f.Name)
		}
		delete(expected, f.Name)
		if f.UncompressedSize64 != uint64(r.Bytes) {
			panic("size mismatch")
		}
		input, err := f.Open()
		must(err)
		data, err := io.ReadAll(io.LimitReader(input, r.Bytes+1))
		must(err)
		must(input.Close())
		if int64(len(data)) != r.Bytes || digest(data) != r.SHA {
			panic("archive mismatch: " + r.Path)
		}
		if mode == "restore" {
			payloads[r.Path] = data
		}
	}
	if len(expected) != 0 {
		panic("archive missing entries")
	}
	if mode == "restore" {
		if len(os.Args) != 5 {
			panic("destination required")
		}
		dest, err := filepath.Abs(os.Args[4])
		must(err)
		_, err = os.Stat(dest)
		if !os.IsNotExist(err) {
			panic("restore destination must not exist")
		}
		must(os.MkdirAll(dest, 0755))
		for _, r := range m.Files {
			path := filepath.Join(dest, r.Path)
			must(os.MkdirAll(filepath.Dir(path), 0755))
			f, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0644)
			must(err)
			_, err = f.Write(payloads[r.Path])
			must(err)
			must(f.Close())
		}
	}
	data, err := os.ReadFile(archive)
	must(err)
	fmt.Printf("Verified %d files; source SHA-256 %s; archive SHA-256 %s\n", m.Count, m.SHA, digest(data))
}

package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func command(t *testing.T, args ...string) (failed bool) {
	t.Helper()
	original := os.Args
	defer func() {
		os.Args = original
		if recover() != nil {
			failed = true
		}
	}()
	os.Args = append([]string{"reference"}, args...)
	main()
	return false
}

func TestArchiveRecoveryAndOverwriteRejection(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	must(os.Mkdir(source, 0755))
	data := []byte("exact reference bytes\r\n")
	must(os.WriteFile(filepath.Join(source, "sample.go"), data, 0644))
	record := Record{Path: "sample.go", Bytes: int64(len(data)), SHA: digest(data)}
	manifest := Manifest{Files: []Record{record}, Count: 1, SHA: digest([]byte(record.Path + "\x00" + record.SHA + "\n"))}
	encoded, err := json.Marshal(manifest)
	must(err)
	manifestPath := filepath.Join(root, "manifest.json")
	must(os.WriteFile(manifestPath, encoded, 0644))
	archive := filepath.Join(root, "reference.zip")
	dest := filepath.Join(root, "restored")
	if command(t, "create", archive, manifestPath, source) || command(t, "verify", archive, manifestPath) || command(t, "restore", archive, manifestPath, dest) {
		t.Fatal("valid archive failed")
	}
	restored, err := os.ReadFile(filepath.Join(dest, "sample.go"))
	must(err)
	if string(restored) != string(data) {
		t.Fatal("recovery altered bytes")
	}
	if !command(t, "restore", archive, manifestPath, dest) {
		t.Fatal("overwrote existing destination")
	}
	must(os.WriteFile(filepath.Join(source, "sample.go"), []byte("changed"), 0644))
	if !command(t, "create", filepath.Join(root, "changed.zip"), manifestPath, source) {
		t.Fatal("accepted changed source")
	}
	must(os.WriteFile(archive, []byte("corrupt"), 0644))
	if !command(t, "verify", archive, manifestPath) {
		t.Fatal("accepted corrupt archive")
	}
}

func TestManifestRejectsTraversalEvenWithMatchingDigest(t *testing.T) {
	for _, path := range []string{"../escape", "/absolute", "C:/drive", "dir/../../escape", "dir\\escape"} {
		record := Record{Path: path, SHA: digest(nil)}
		manifest := Manifest{Files: []Record{record}, Count: 1, SHA: digest([]byte(path + "\x00" + record.SHA + "\n"))}
		encoded, err := json.Marshal(manifest)
		must(err)
		file := filepath.Join(t.TempDir(), "manifest.json")
		must(os.WriteFile(file, encoded, 0644))
		rejected := false
		func() { defer func() { rejected = recover() != nil }(); readManifest(file) }()
		if !rejected {
			t.Fatalf("accepted %s", path)
		}
	}
}

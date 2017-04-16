CREATE TABLE "track" (
    "path" TEXT,
    "modified_at" INTEGER NOT NULL,

    "duration" INTEGER NOT NULL,
    "title" TEXT NOT NULL,
    "rating" INTEGER,
    "release" TEXT,

    -- Albums could be in their own table if its PK would not be so complex:
    -- title + multiple artists.
    "album_title" TEXT,
    "album_disc" INTEGER,
    "album_track" INTEGER,

    PRIMARY KEY ("path")
        ON CONFLICT REPLACE
);

-- A track has zero or more artists.
CREATE TABLE "track_artist" (
    "track_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "type" TEXT,

    PRIMARY KEY ("track_path", "name")
        ON CONFLICT REPLACE,
    FOREIGN KEY ("track_path") REFERENCES "track"("path")
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK ("type" IN ("album", "remixer"))
);

-- A track has zero or more genres.
CREATE TABLE "track_genre" (
    "track_path" TEXT,
    "genre" TEXT NOT NULL,

    PRIMARY KEY ("track_path", "genre")
        ON CONFLICT REPLACE,
    FOREIGN KEY ("track_path") REFERENCES "track"("path")
        ON UPDATE CASCADE ON DELETE CASCADE
);

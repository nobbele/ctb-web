CREATE TABLE IF NOT EXISTS 'charts' (
    id INTEGER NOT NULL,
    title TEXT NOT NULL,
);

CREATE TABLE IF NOT EXISTS 'difficulties' (
    chart_id INTEGER NOT NULL,
    id INTEGER NOT NULL,
    title TEXT NOT NULL,

    CONSTRAINT fk_chart
      FOREIGN KEY(chart_id)
	  REFERENCES charts(id)
);

INSERT INTO charts VALUES (1, "Kizuato");
INSERT INTO difficulties VALUES(1, 1, "Platter");
INSERT INTO difficulties VALUES(1, 2, "Ascendance's Rain");
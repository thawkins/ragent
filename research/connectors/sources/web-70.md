# Web source

- URL: https://seatunnel.incubator.apache.org/docs/2.3.12/connector-v2/source/ObsFile
- Title: ObsFile | Apache SeaTunnel
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:59.631630130+00:00
- Relevance: Medium-high - snippet matches query


```text
Apache SeaTunnel 2.3.12’s ObsFile source connector supports Spark, Flink, and SeaTunnel Zeta and reads data from Huawei Cloud OBS file systems. Spark/Flink require an integrated Hadoop cluster (tested Hadoop 2.x), while SeaTunnel Engine auto-integrates the Hadoop jar; internally it uses HDFS protocol for OBS and requires Hadoop 2.9.X+. Required jars are hadoop-huaweicloud ≥3.1.1.29, esdk-obs-java ≥3.19.7.3, okhttp ≥3.11.0, and okio ≥1.14.0, copied to `$SEATUNNEL_HOME/plugins/jdbc/lib/` and `$SEATUNNEL_HOME/lib/`. Required options include `path`, `file_format_type`, `bucket`, `access_key`, `access_secret`, `endpoint`, and `read_columns`; supported formats are text, csv, parquet, orc, json, and excel. Optional options include `delimiter` default `\001`, `row_delimiter` default `\n`, `parse_partition_from_path` default true, `skip_header_row_number` default 0, `date_format` default `yyyy-MM-dd`, `datetime_format` default `yyyy-MM-dd HH:mm:ss`, `time_format` default `HH:mm:ss`, `filename_extension`, `schema`, `sheet_name`, and file modification time filters; JSON requires schema, parquet/orc infer schema, text/csv schema is optional, and column projection is supported for text/json/csv/orc/parquet/excel.
```

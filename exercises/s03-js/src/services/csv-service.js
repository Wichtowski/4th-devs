const isConstructibleClass = (value) => {
  if (typeof value !== "function") {
    return false;
  }

  const prototype = value.prototype;
  if (!prototype || typeof prototype !== "object") {
    return false;
  }

  return Object.getOwnPropertyNames(prototype).length > 1;
};

const parseCsv = (csvText) => {
  const rows = [];
  let row = [];
  let field = "";
  let inQuotes = false;

  for (let i = 0; i < csvText.length; i += 1) {
    const char = csvText[i];

    if (inQuotes) {
      if (char === '"') {
        if (csvText[i + 1] === '"') {
          field += '"';
          i += 1;
        } else {
          inQuotes = false;
        }
      } else {
        field += char;
      }
      continue;
    }

    if (char === '"') {
      inQuotes = true;
      continue;
    }

    if (char === ",") {
      row.push(field);
      field = "";
      continue;
    }

    if (char === "\r") {
      if (csvText[i + 1] === "\n") {
        i += 1;
      }
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      continue;
    }

    if (char === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      continue;
    }

    field += char;
  }

  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }

  return rows;
};

export class CsvService {
  static readRecords(csvText) {
    const rows = parseCsv(csvText);
    if (rows.length === 0) {
      return [];
    }

    const [headers, ...dataRows] = rows;
    return dataRows
      .filter((row) => row.some((value) => value.trim() !== ""))
      .map((row) => {
        const record = {};
        for (let index = 0; index < headers.length; index += 1) {
          record[headers[index]] = row[index] ?? "";
        }
        return record;
      });
  }

  static readRecordsAs(csvText, createRecord, ...args) {
    const records = CsvService.readRecords(csvText);
    if (typeof createRecord !== "function") {
      throw new Error("createRecord must be a function");
    }

    return records.map((record) => {
      if (isConstructibleClass(createRecord)) {
        return new createRecord(record, ...args);
      }

      return createRecord(record, ...args);
    });
  }
}

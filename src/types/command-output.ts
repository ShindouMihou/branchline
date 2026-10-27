export interface CommandOutput {
  stepIndex:number;
  stdout:string;
  stderr:string;
  stdoutTruncated:boolean;
  stderrTruncated:boolean;
}

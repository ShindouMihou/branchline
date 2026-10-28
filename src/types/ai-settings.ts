export interface AiSettingsConnection {
  id:string;
  name:string;
  provider?:string;
  active:boolean;
  readSource?:boolean;
}
export interface AiSettingsView {
  enabled:boolean;
  autoOpen?:boolean;
  notifications?:boolean;
  autoApprovalCount:number;
  clients:AiSettingsConnection[];
  busy:boolean;
  selectedClientId:string;
  confirmDisconnect:boolean;
  connectionMenuOpen?:boolean;
}

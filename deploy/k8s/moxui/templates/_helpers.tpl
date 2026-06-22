{{/*
MoxUI Helm chart helpers.
Expand the name of the chart.
*/}}
{{- define "moxui.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "moxui.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "moxui.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "moxui.labels" -}}
helm.sh/chart: {{ include "moxui.chart" . }}
{{ include "moxui.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "moxui.selectorLabels" -}}
app.kubernetes.io/name: {{ include "moxui.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "moxui.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "moxui.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
ConfigMap name
*/}}
{{- define "moxui.configmapName" -}}
{{ include "moxui.fullname" . }}-config
{{- end }}

{{/*
Secret name
*/}}
{{- define "moxui.secretName" -}}
{{- if .Values.existingSecret }}
{{- .Values.existingSecret }}
{{- else }}
{{- include "moxui.fullname" . }}-secrets
{{- end }}
{{- end }}

{{/*
ServiceMonitor name
*/}}
{{- define "moxui.servicemonitorName" -}}
{{ include "moxui.fullname" . }}
{{- end }}

{{/*
Render clusters YAML — converts array of cluster definitions to valid YAML
with password refs pointing to the Secret.
*/}}
{{- define "moxui.clustersYaml" -}}
{{- range $i, $cluster := .Values.clusters }}
  - name: {{ $cluster.name | quote }}
    url: {{ $cluster.url | quote }}
    username: {{ $cluster.username | quote }}
    password: "${MOXUI_CLUSTER_{{ $i }}_PASSWORD}"
    realm: {{ $cluster.realm | quote }}
    insecure_skip_verify: {{ $cluster.insecure_skip_verify }}
{{- end }}
{{- end }}

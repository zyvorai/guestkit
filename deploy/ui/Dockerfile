# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
FROM python:3.12-alpine
WORKDIR /ui
COPY . /ui
EXPOSE 8443
CMD ["python3","serve-https.py","--port","8443","--dir","/ui"]

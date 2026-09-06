FROM python:3.12-alpine
WORKDIR /ui
COPY . /ui
EXPOSE 8443
CMD ["python3","serve-https.py","--port","8443","--dir","/ui"]

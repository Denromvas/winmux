Це папка, що буде змонтована у /workspace всередині гостьового Linux.

Перевірка з гостя:
  sudo mount -t 9p -o trans=virtio,version=9p2000.L workspace /workspace
  ls /workspace
  cat /workspace/README.txt

Зміни тут видимі обом сторонам у real-time.

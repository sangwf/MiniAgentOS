import socket

def main():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(10)
    print("UDP interactive. Type messages, 'quit' to exit.")
    while True:
        try:
            line = input("> ")
        except EOFError:
            break
        if line.strip() in ("quit", "exit"):
            break
        line_str = line.strip()
        if line_str.startswith(("http://", "https://")) or line_str.startswith(("get ", "post ")):
            s.settimeout(60)
            chunks = {}
            got_ok = False
            body_chunks = {}
            json_chunks = {}
        else:
            s.settimeout(5)
            chunks = None
            got_ok = False
            body_chunks = None
            json_chunks = None
        data = line.encode("utf-8", errors="ignore")
        s.sendto(data, ("127.0.0.1", 5555))
        got = False
        while True:
            try:
                resp, addr = s.recvfrom(2048)
                print("recv:", resp, "from", addr)
                got = True
                if chunks is not None:
                    marker = resp.find(b"http chunk ")
                    if marker != -1:
                        idx_start = marker + len(b"http chunk ")
                        idx_end = resp.find(b" ", idx_start)
                        if idx_end != -1:
                            idx_bytes = resp[idx_start:idx_end]
                            try:
                                idx = int(idx_bytes.decode("ascii"))
                                chunks[idx] = resp[idx_end + 1 :]
                            except ValueError:
                                pass
                    marker = resp.find(b"body chunk ")
                    if marker != -1 and body_chunks is not None:
                        idx_start = marker + len(b"body chunk ")
                        idx_end = resp.find(b" ", idx_start)
                        if idx_end != -1:
                            idx_bytes = resp[idx_start:idx_end]
                            try:
                                idx = int(idx_bytes.decode("ascii"))
                                body_chunks[idx] = resp[idx_end + 1 :]
                            except ValueError:
                                pass
                    marker = resp.find(b"json chunk ")
                    if marker != -1 and json_chunks is not None:
                        idx_start = marker + len(b"json chunk ")
                        idx_end = resp.find(b" ", idx_start)
                        if idx_end != -1:
                            idx_bytes = resp[idx_start:idx_end]
                            try:
                                idx = int(idx_bytes.decode("ascii"))
                                json_chunks[idx] = resp[idx_end + 1 :]
                            except ValueError:
                                pass
                    if b"http ok " in resp or b"http fail " in resp:
                        got_ok = True
            except TimeoutError:
                if not got:
                    print("timeout")
                break
        if chunks is not None and (chunks or got_ok):
            assembled = b"".join(chunks[k] for k in sorted(chunks))
            if assembled:
                print("----- http content (up to 4KB) -----")
                print(assembled.decode("utf-8", errors="replace"))
                print("----- end -----")
        if body_chunks is not None and body_chunks:
            assembled = b"".join(body_chunks[k] for k in sorted(body_chunks))
            if assembled:
                print("----- body content (up to 4KB) -----")
                print(assembled.decode("utf-8", errors="replace"))
                print("----- end -----")
        if json_chunks is not None and json_chunks:
            assembled = b"".join(json_chunks[k] for k in sorted(json_chunks))
            if assembled:
                print("----- json content (up to 4KB) -----")
                print(assembled.decode("utf-8", errors="replace"))
                print("----- end -----")

if __name__ == "__main__":
    main()

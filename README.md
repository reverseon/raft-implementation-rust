# Tugas Besar 1 - Connsensus Protocol: Raft
## IF3230 - Sistem Paralel dan Terdistribusi
### Deskripsi Program
Kami menggunakan bahasa Rust untuk program Tugas Besar 1 ini. Program merupakan distributed message queue/dequeue dengan tipe data pada queue berupa string. Implementasi yang dilakukan berupa Membership Change, Log Replication, Heartbeat, dan Leader Election. Terdapat 2 interface untuk server yang dapat digunakan oleh client. Program melakukan logging pada terminal terhadap aksi dari server.
### SETUP
1. Clone repository dengan command `git clone`
2. Change directory ke folder dengan `cd`
3. Jalankan command `cargo build --release`

### Running the Program
Berikut merupakan langkah-langkah untuk menjalankan program:

Raft Node Server: 
- `./target/release/raft-node-server [PORT]`
  - e.g. `./target/release/raft-node-server 24341`

Raft Client
- `./target/release/raft-client`

Dashboard
1. `./target/release/http-interface 1227`
    - Apabila ingin mengganti dari 1227 menjadi yang lain, dapar dilakukan dengan mengubah address endpoint di `src/ui/App.tsx`
2. `cd ui`
3. `yarn dev`
4. buka `localhost:5173`

\* Dijalankan menggunakan Ubuntu 22.04.2 (VirtualBox with Hyper-V) dengan alokasi 4 core 8 thread.

### K03 - Kelompok 3: tenshi-otonari
##### Anggota Kelompok:
- 13520127 Adzka Ahmadetya Zaidan
- 13520144 Zayd Muhammad Kawakibi Zuhri
- 13520157 Thirafi Najwan Kurniatama
- 13520161 M Syahrul Surya Putra

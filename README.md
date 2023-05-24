# Tugas Besar 1 - Connsensus Protocol: Raft
## IF3230 - Sistem Paralel dan Terdistribusi
### Deskripsi Program
Kami menggunakan bahasa Rust untuk program Tugas Besar 1 ini. Program merupakan distributed message queue/dequeue dengan tipe data pada queue berupa string. Implementasi yang dilakukan berupa Membership Change, Log Replication, Heartbeat, dan Leader Election. Terdapat 2 interface untuk server yang dapat digunakan oleh client. Program melakukan logging pada terminal terhadap aksi dari server. Setup raft cluster ini memiliki election timeout dari 15 detik hingga 30 detik dan mengirimkan heartbeat setiap 1 detik dikarenakan keterbatasan sumber daya lingkungan testing. Program ini juga memiliki dashboard yang dapat digunakan untuk melihat status dari setiap node di cluster raft.
### SETUP
1. Clone repository dengan command `git clone`
2. Change directory ke folder dengan `cd`
3. Jalankan command `cargo build --release`

### Running the Program
Berikut merupakan langkah-langkah untuk menjalankan program:

Raft Node Server: 
- `./target/release/raft-node-server [PORT]`
  - e.g. `./target/release/raft-node-server 24341`

Command ini saat pertama kali dijalankan akan membuat dua folder yaitu:
- `cfg` yang berisikan file `config.json` yang menyimpan address dari node di cluster raft
  - setiap node yang baru dijalankan otomatis akan masuk ke file ini. Namun jika node dihentikan, tidak akan langsung dihapus.
- `logs` berisikan persistent data dari setiap node.

Jika kedua folder ini tidak kosong, maka cluster akan melanjutkan dari data yang ada pada folder tersebut.

Raft Client
- `./target/release/raft-client`

Dashboard
1. `./target/release/http-interface 1227`
    - Apabila ingin mengganti dari 1227 menjadi yang lain, dapar dilakukan dengan mengubah address endpoint di `src/ui/src/App.tsx`
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

import java.net.*;
import java.io.*;

public class MulticastReceiver {

    public static void main(String[] args) {
        String multicastAddress = "224.0.0.1"; // 组播地址
        int port = 5000; // 端口号

        try {
            // 创建 MulticastSocket 并加入组播组
            MulticastSocket socket = new MulticastSocket(port);
            InetAddress group = InetAddress.getByName(multicastAddress);
            
            // 加入到组播组
            socket.joinGroup(group);

            System.out.println("已加入组播组: " + multicastAddress + "：" + port);

            byte[] buffer = new byte[256]; // 缓冲区，用于接收数据包

            while (true) {
                DatagramPacket packet = new DatagramPacket(buffer, buffer.length);
                
                // 接收组播消息
                socket.receive(packet);

                // 输出接收到的消息
                String message = new String(packet.getData(), 0, packet.getLength());
                System.out.println("接收到的消息: " + message);
            }

        } catch (IOException e) {
            e.printStackTrace();
        }
    }
}



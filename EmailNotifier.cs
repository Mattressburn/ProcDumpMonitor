using System.Net;
using System.Net.Mail;
using System.Net.Sockets;

namespace ProcDumpMonitor;

public static class EmailNotifier
{
    /// <summary>Send a dump-notification email.</summary>
    public static void SendDumpNotification(Config cfg, string dumpFilePath)
    {
        string subject = $"[ProcDump] Dump created for {cfg.TargetName} on {Environment.MachineName}";
        string body =
            $"A process dump was captured.\r\n\r\n" +
            $"Target:     {cfg.TargetName}\r\n" +
            $"Computer:   {Environment.MachineName}\r\n" +
            $"Dump File:  {dumpFilePath}\r\n" +
            $"Timestamp:  {DateTime.Now:yyyy-MM-dd HH:mm:ss}\r\n";

        Send(cfg, subject, body);
    }

    /// <summary>Send a test email.</summary>
    public static void SendTestEmail(Config cfg)
    {
        string subject = $"[ProcDump] Test email from {Environment.MachineName}";
        string body =
            $"This is a test email from ProcDump Monitor.\r\n\r\n" +
            $"Computer:   {Environment.MachineName}\r\n" +
            $"Timestamp:  {DateTime.Now:yyyy-MM-dd HH:mm:ss}\r\n";

        Send(cfg, subject, body);
    }

    /// <summary>Core send helper.</summary>
    private static void Send(Config cfg, string subject, string body)
    {
#pragma warning disable CS0618 // SmtpClient is obsolete but fine for simple relay usage
        using var client = new SmtpClient(cfg.SmtpServer, cfg.SmtpPort);
        client.EnableSsl = cfg.UseSsl;
        client.Timeout = 30_000;

        if (!string.IsNullOrWhiteSpace(cfg.SmtpUsername))
        {
            string password = cfg.GetPassword();
            client.Credentials = new NetworkCredential(cfg.SmtpUsername, password);
        }
        else
        {
            client.UseDefaultCredentials = false;
            client.Credentials = null;
        }

        using var msg = new MailMessage(cfg.FromAddress, cfg.ToAddress, subject, body);
        client.Send(msg);
#pragma warning restore CS0618
    }

    /// <summary>Validate SMTP connectivity using a raw TCP connection (like Test-NetConnection).</summary>
    public static (bool Success, string Message) ValidateSmtpConnectivity(string server, int port, int timeoutMs = 5000)
    {
        try
        {
            using var tcp = new TcpClient();
            var connectTask = tcp.ConnectAsync(server, port);
            if (!connectTask.Wait(timeoutMs))
            {
                return (false, $"Connection to {server}:{port} timed out after {timeoutMs}ms.");
            }

            if (tcp.Connected)
            {
                // Try to read the SMTP banner
                using var stream = tcp.GetStream();
                stream.ReadTimeout = timeoutMs;
                byte[] buffer = new byte[1024];
                int bytesRead = stream.Read(buffer, 0, buffer.Length);
                string banner = System.Text.Encoding.ASCII.GetString(buffer, 0, bytesRead).Trim();
                return (true, $"Connected to {server}:{port}\r\nBanner: {banner}");
            }

            return (false, $"Failed to connect to {server}:{port}.");
        }
        catch (Exception ex)
        {
            return (false, $"Connection failed: {ex.Message}");
        }
    }
}

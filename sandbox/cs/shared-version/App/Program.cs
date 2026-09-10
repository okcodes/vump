// Both assemblies are built from one <Version>, so both report the same
// string. An executable that printed only its own would say nothing about the
// libraries shipped beside it.
using System.Reflection;

static string VersionOf(Assembly assembly) =>
    assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion
    ?? "unknown";

Console.WriteLine($"App {VersionOf(Assembly.GetExecutingAssembly())}");
Console.WriteLine($"Lib {VersionOf(typeof(Lib.Greeting).Assembly)}");
Console.WriteLine(Lib.Greeting.Text);

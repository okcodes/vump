// Prints the informational version, which is the one <Version> feeds and the
// only one that keeps a pre-release suffix: AssemblyVersion is four numeric
// parts and drops it.
using System.Reflection;

var assembly = Assembly.GetExecutingAssembly();
var version = assembly
    .GetCustomAttribute<AssemblyInformationalVersionAttribute>()
    ?.InformationalVersion;

Console.WriteLine($"ProjectA {version}");

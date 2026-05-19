using System;
using System.Collections.Generic;

namespace Ariadne.Sample
{
    public interface IStep
    {
        int Next(int current);
    }

    public enum Side
    {
        Left,
        Right,
    }

    public struct Point
    {
        public int X;
        public int Y;
    }

    public record Vec(int X, int Y);

    public class Counter
    {
        public int Value;

        public int Increment()
        {
            Value += 1;
            return Value;
        }

        public static int Tick(int v)
        {
            var list = new List<int>();
            list.Add(v);
            return Console.In.Read();
        }
    }
}

using Plots
using Metal
gr(fmt=:png)

N = 3

function rmall(; ball=false)
    for i in 1:N
        ball && rm("images", force=true, recursive=true)
        rm("test$i.png", force=true)
    end
    rm("metadata.json", force=true)
end

function plt(j)
    n = 10 * j
    x = n / 1.78 |> round |> Int64
    plotmandelbrot(m) = heatmap(m; c=reverse(cgrad(:jet1)),
        size=(x, n), colorbar=false, ticks=false, frame=false, margin=0 * Plots.PlotMeasures.mm)

    function mandelbrot(c; maxiters=2^10, threshold_abs2=2^10)
        z = zero(c)
        for i in 1:maxiters
            z = z * z + c
            abs2(z) ≥ threshold_abs2 && return i
        end
        maxiters + 1
    end

    x32 = range(-0.714689f0, -0.714679f0; length=n)
    y32 = range(0.299872f0, 0.299882f0; length=n)
    c32 = MtlArray(complex.(x32', y32))

    m = collect(mandelbrot.(c32))
    p = plotmandelbrot(m)
    savefig(p, "test1.png")
end


rmall(ball=true)
mkdir("images")
for i in 1:N
    # プロット
    plt(i)

    # コピー
    for j in 2:2
        cp("test1.png", "test$j.png")
    end

    # メタデータ
    json = """{"title":"Test Title $i","creator":"Test Creator $i","publisher":"Test Publisher $i","date":"2000-01-01T00:00:00Z","is_rtl":$((i % 2) == 0 ? "true" : "false")}"""
    write("metadata.json", json)

    # 圧縮
    run(`sh tar.sh $i`)

    # 移動
    mv("test$i.tar.gz", "images/test$i.tar.gz")

    # 削除
    rmall()
end

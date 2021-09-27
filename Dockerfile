FROM ekidd/rust-musl-builder:stable as builder

ARG APP_NAME=cereal

WORKDIR /home/rust/
RUN cargo new --bin ${APP_NAME} 
WORKDIR /home/rust/${APP_NAME}
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs

ADD ./src ./src

RUN rm ./target/x86_64-unknown-linux-musl/release/deps/${APP_NAME}*
RUN cargo build --release


FROM ubuntu:impish

ARG APP=/usr/src/app
ARG APP_NAME=cereal-convert

ENV DEBIAN_FRONTEND=noninteractive
ENV APP_USER=appuser

RUN addgroup --system $APP_USER \
    && adduser --system --ingroup $APP_USER $APP_USER

RUN ln -fs /usr/share/zoneinfo/America/New_York /etc/localtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates calibre


COPY --from=builder /home/rust/${APP_NAME}/target/x86_64-unknown-linux-musl/release/${APP_NAME} ${APP}/${APP_NAME}

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

EXPOSE 3000

CMD ["./cereal-convert"]
